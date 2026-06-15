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
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::opaque::HostOpaque;
use crate::vm::VmError;
use crate::word::Word;

/// Error for decoding a flat composite whose arena body no longer resolves
/// because a `RESET` advanced the epoch (B28 P3 item 5 C3). Under the
/// read-before-resume contract a yielded or returned composite stays
/// arena-resident, so the host must decode it before the next `resume()` or
/// before dropping the VM; decoding afterward returns this clean error rather
/// than panicking. Public because the `#[derive(KeleusmaType)]` macro emits
/// calls to it.
pub fn stale_flat_decode() -> VmError {
    VmError::TypeError(alloc::string::String::from(
        "flat composite body read after the arena was reset; decode a yielded or \
         returned composite before the next resume (read-before-resume)",
    ))
}

/// Resolution context for decoding a flat composite's reference fields at
/// the host boundary (B28 P3).
///
/// A flat `Text` field is a two-word arena `(ptr, len)` reference that is
/// rebuilt into a `KString` against the arena epoch; a flat `Opaque` field
/// is an index into the VM's ephemeral opaque registry. This context
/// supplies the arena and the registry so [`KeleusmaType::from_value_ctx`]
/// and [`KeleusmaType::from_flat_bytes_ctx`] can resolve them.
///
/// Like every arena read, a decoded reference is valid only until the next
/// `resume`/`RESET`. The host uses or copies it before then, which is the
/// same use-before-`resume` discipline that already governs `KStr`.
pub struct RefContext<'a> {
    /// The VM's arena, used to rebuild a `KString` from a flat `Text`
    /// field's `(ptr, len)`.
    pub arena: &'a keleusma_arena::Arena,
    /// The VM's ephemeral opaque registry, indexed by a flat `Opaque`
    /// field to recover the host reference.
    pub opaques: &'a [Arc<dyn HostOpaque>],
    /// The module's word byte width, used to read flat fields at the width
    /// the body was packed with. This is the module's declared width, not
    /// the host type's `Word`; the two differ on a narrow-word build, where
    /// the bundled `i64` VM runs a module whose words are narrower.
    pub word_bytes: usize,
    /// The module's float byte width, paired with `word_bytes`.
    pub float_bytes: usize,
    /// The originating arena epoch of the flat composite being decoded
    /// (B28 P3 item 1). A flat `Text` field's `KString` is rebuilt against
    /// this epoch, not the current arena epoch, so a decode after a `RESET`
    /// resolves to a clean `Stale` outcome rather than dereferencing
    /// reclaimed memory. It is the composite body's `ref_epoch`; for a
    /// non-composite decode it is the current arena epoch (a bare `KStr`
    /// carries its own epoch and ignores this field).
    pub ref_epoch: u64,
}

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

    /// Like [`KeleusmaType::into_value`] but building any flat composite body
    /// directly in the arena rather than the global heap (B28 P3 item 2,
    /// Increment 3).
    ///
    /// The default materialises through `into_value` then migrates the body to
    /// the arena via `into_arena_body`, which is correct for every type (a
    /// scalar, string, opaque, or boxed value is returned unchanged). A flat
    /// composite type overrides this to pack straight into the arena through
    /// the `*_in_arena` constructors, skipping the transient top-level
    /// global-heap body. The native-result boundary calls this so an ephemeral
    /// native return carries no global-heap composite body across a `loop`
    /// iteration's `RESET`. The widths are the runtime's own, exactly as
    /// `into_value` uses, so the packed bytes are identical to the
    /// `into_value`-then-`into_arena_body` path; only the residence differs.
    fn into_value_ctx(self, ctx: &RefContext<'_>) -> Result<GenericValue<W, F>, VmError> {
        self.into_value().into_arena_body(ctx.arena).map_err(|_| {
            VmError::OutOfArena(alloc::string::String::from(
                "arena exhausted building a native result composite body",
            ))
        })
    }

    /// The flat-composite scalar kind this type occupies when it is a
    /// tuple field, or `None` when it is not a flat-eligible scalar
    /// (B28 P2).
    ///
    /// Used to read an element out of a flat tuple body at the host
    /// boundary, where the value is pure bytes and the Rust type
    /// supplies the layout. The default is `None`, treated as a
    /// non-flat field, so existing external implementations remain
    /// valid without change. The `f64` impl overrides this to
    /// `Some(Float)` (B28 P3 item 5): float fields are flat, and a
    /// float-bearing composite is compared field-wise by the compiler so
    /// the byte residence preserves the `+0.0`/`-0.0` and `NaN` semantics.
    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        None
    }

    /// The flat byte size this type occupies inside a flat composite body,
    /// or `None` when it is not flat-eligible (B28 P2 nested inlining).
    ///
    /// A flat-eligible scalar returns its scalar size (the default, derived
    /// from [`KeleusmaType::flat_field_kind`]). A flat composite (a derived
    /// struct or enum, or a tuple or array of flat fields) overrides this to
    /// return its total flat body size, so it can be read from and written
    /// to a parent body inline. A type that returns `None` keeps the boxed
    /// representation and is not inlined.
    fn flat_byte_size(word_bytes: usize, float_bytes: usize) -> Option<usize> {
        Self::flat_field_kind().map(|k| k.size_in_bytes(word_bytes, float_bytes))
    }

    /// Reconstruct the Rust type from a flat byte slice that holds exactly
    /// this type's flat body (B28 P2 nested inlining).
    ///
    /// The default reads a single flat scalar of [`KeleusmaType::flat_field_kind`]
    /// from the start of `bytes`; a type with no flat scalar kind returns a
    /// [`VmError::TypeError`]. Flat composites override this to read their
    /// fields at packed offsets, recursing through nested composites.
    fn from_flat_bytes(
        bytes: &[u8],
        word_bytes: usize,
        float_bytes: usize,
    ) -> Result<Self, VmError> {
        match Self::flat_field_kind() {
            Some(kind) => {
                let v = GenericValue::read_scalar_le(bytes, 0, kind, word_bytes, float_bytes);
                Self::from_value(&v)
            }
            None => Err(VmError::TypeError(alloc::string::String::from(
                "type has no flat byte representation",
            ))),
        }
    }

    /// Like [`KeleusmaType::from_value`] but with a [`RefContext`] for
    /// resolving reference fields (B28 P3). The default ignores the context
    /// and delegates, so scalar and value-only types need no change.
    /// `String` and `Arc<dyn HostOpaque>` override it, and the derive macro
    /// generates an override that threads the context to each field.
    fn from_value_ctx(v: &GenericValue<W, F>, ctx: &RefContext<'_>) -> Result<Self, VmError> {
        let _ = ctx;
        Self::from_value(v)
    }

    /// Like [`KeleusmaType::from_flat_bytes`] but with a [`RefContext`] for
    /// resolving reference fields (B28 P3). The default delegates to the
    /// context-free reader; composites with reference fields override it
    /// (via the derive macro) to thread the context to each field.
    fn from_flat_bytes_ctx(
        bytes: &[u8],
        word_bytes: usize,
        float_bytes: usize,
        ctx: &RefContext<'_>,
    ) -> Result<Self, VmError> {
        let _ = ctx;
        Self::from_flat_bytes(bytes, word_bytes, float_bytes)
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

    // A float is flat (B28 P3 item 5): it occupies `float_bytes` in a flat
    // composite body and is read/written little-endian by the default
    // `flat_byte_size`/`from_flat_bytes`. A float-bearing composite is
    // compared field-wise by the compiler, so the flat residence keeps its
    // IEEE equality semantics. This makes host-built and script-built float
    // composites share the flat representation that equality and access rely
    // on.
    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Float)
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

// -- Reference types (Text, opaque): host-boundary decode (B28 P3) --

/// `String` marshals the surface `Text` type. A host receives an owned
/// `String` copy and produces a static string. Reading from a flat `Text`
/// field (a two-word arena `(ptr, len)`) or a dynamic `KStr` requires a
/// [`RefContext`]; the context-free paths handle only the owning
/// `StaticStr` and otherwise direct the caller to `Vm::decode`.
impl<W: Word, F: Float> KeleusmaType<W, F> for alloc::string::String {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::StaticStr(s) => Ok(s.clone()),
            GenericValue::KStr(_) => Err(VmError::TypeError(alloc::string::String::from(
                "dynamic string requires a resolution context; decode through Vm::decode",
            ))),
            other => Err(VmError::TypeError(format!(
                "expected Text, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::StaticStr(self)
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Text)
    }

    fn from_flat_bytes(
        _bytes: &[u8],
        _word_bytes: usize,
        _float_bytes: usize,
    ) -> Result<Self, VmError> {
        // A flat Text field is a (ptr, len) arena reference; resolving it
        // needs the arena epoch. Direct the caller to the context path
        // rather than dereferencing without it.
        Err(VmError::TypeError(alloc::string::String::from(
            "flat Text field requires a resolution context; decode through Vm::decode",
        )))
    }

    fn from_value_ctx(v: &GenericValue<W, F>, ctx: &RefContext<'_>) -> Result<Self, VmError> {
        match v {
            GenericValue::StaticStr(s) => Ok(s.clone()),
            GenericValue::KStr(ks) => {
                ks.get(ctx.arena)
                    .map(alloc::string::String::from)
                    .map_err(|_| {
                        VmError::TypeError(alloc::string::String::from(
                            "dynamic string is stale (arena reset since it was produced)",
                        ))
                    })
            }
            other => Err(VmError::TypeError(format!(
                "expected Text, got {}",
                other.type_name()
            ))),
        }
    }

    fn from_flat_bytes_ctx(
        bytes: &[u8],
        word_bytes: usize,
        _float_bytes: usize,
        ctx: &RefContext<'_>,
    ) -> Result<Self, VmError> {
        let read_word = |o: usize| -> usize {
            let mut buf = [0u8; 8];
            buf[..word_bytes].copy_from_slice(&bytes[o..o + word_bytes]);
            u64::from_le_bytes(buf) as usize
        };
        let ptr = read_word(0);
        let len = read_word(word_bytes);
        // SAFETY: the (ptr, len) was packed from a KString issued under the
        // composite body's originating epoch (`ctx.ref_epoch`). Rebuilding
        // with that epoch (not the current arena epoch) means the `get`
        // below dereferences the region only while the arena epoch still
        // matches, and yields a clean Stale error once a RESET has advanced
        // it (B28 P3 item 1).
        let ks = unsafe { crate::kstring::KString::from_raw_parts(ptr, len, ctx.ref_epoch) };
        ks.get(ctx.arena)
            .map(alloc::string::String::from)
            .map_err(|_| {
                VmError::TypeError(alloc::string::String::from(
                    "flat Text field is stale (arena reset since it was produced)",
                ))
            })
    }
}

/// An opaque host reference is a flat pass-through: the host receives the
/// `Arc` and downcasts it through [`dyn HostOpaque::downcast_ref`]. In a
/// flat body it is a one-word index into the VM's ephemeral opaque
/// registry, resolved through the [`RefContext`].
impl<W: Word, F: Float> KeleusmaType<W, F> for Arc<dyn HostOpaque> {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Opaque(o) => Ok(Arc::clone(o)),
            other => Err(VmError::TypeError(format!(
                "expected opaque, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Opaque(self)
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Opaque)
    }

    fn from_flat_bytes(
        _bytes: &[u8],
        _word_bytes: usize,
        _float_bytes: usize,
    ) -> Result<Self, VmError> {
        Err(VmError::TypeError(alloc::string::String::from(
            "flat opaque field requires a resolution context; decode through Vm::decode",
        )))
    }

    fn from_flat_bytes_ctx(
        bytes: &[u8],
        word_bytes: usize,
        _float_bytes: usize,
        ctx: &RefContext<'_>,
    ) -> Result<Self, VmError> {
        let mut buf = [0u8; 8];
        buf[..word_bytes].copy_from_slice(&bytes[0..word_bytes]);
        let index = u64::from_le_bytes(buf) as usize;
        ctx.opaques.get(index).map(Arc::clone).ok_or_else(|| {
            VmError::InvalidBytecode(alloc::string::String::from(
                "opaque field index does not resolve (stale or out of range)",
            ))
        })
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

    fn from_value_ctx(v: &GenericValue<W, F>, ctx: &RefContext<'_>) -> Result<Self, VmError> {
        match v {
            GenericValue::None => Ok(Option::None),
            other => T::from_value_ctx(other, ctx).map(Some),
        }
    }

    fn into_value_ctx(self, ctx: &RefContext<'_>) -> Result<GenericValue<W, F>, VmError> {
        // `Some(t)` is the bare inner value (the runtime treats any non-`None`
        // as `Some`), so recurse into the inner type's arena-direct builder so
        // a `Some(composite)` native result is arena-resident (B28 P3 item 2,
        // Increment 3).
        match self {
            Some(t) => t.into_value_ctx(ctx),
            Option::None => Ok(GenericValue::None),
        }
    }
}

// -- Fixed-length arrays --

impl<W: Word, F: Float, T: KeleusmaType<W, F> + Clone, const N: usize> KeleusmaType<W, F>
    for [T; N]
{
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        use crate::bytecode::ArrayBody;
        match v {
            GenericValue::Array(ArrayBody::Boxed(items)) => {
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
            // A flat array body is pure bytes; the element type `T` supplies
            // its flat byte size so each element (a scalar or a nested flat
            // composite) is read at its packed offset (B28 P2). Runtime
            // widths match the module widths on the bundled runtime.
            GenericValue::Array(ArrayBody::Flat(fc)) => {
                let word_bytes = (1usize << <W as Word>::BITS_LOG2) / 8;
                let float_bytes = (1usize << <F as Float>::BITS_LOG2) / 8;
                Self::from_flat_bytes(fc.as_bytes(), word_bytes, float_bytes)
            }
            other => Err(VmError::TypeError(format!(
                "expected array, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        // Route through the shared constructor so a host-built array has the
        // same representation as a script-built one of the same type, which
        // array equality relies on (B28 P2).
        GenericValue::array_with_widths(
            self.into_iter().map(|t| t.into_value()).collect(),
            (1usize << <W as Word>::BITS_LOG2) / 8,
            (1usize << <F as Float>::BITS_LOG2) / 8,
        )
    }

    fn into_value_ctx(self, ctx: &RefContext<'_>) -> Result<GenericValue<W, F>, VmError> {
        // Build the flat array body directly in the arena, eliminating the
        // top-level global-heap `Inline` (B28 P3 item 2, Increment 3). Element
        // values and the packing widths are the host runtime's own, exactly as
        // `into_value` uses, so the body is byte-identical to the prior
        // `into_value`-then-`into_arena_body` path and the result decodes the
        // same way through `from_value`. (A narrower *module* word would call
        // for module-width packing here, but that conflicts with the
        // runtime-width `from_value` host decoder; reconciling the two is the
        // separate narrow-word composite-width item noted in REVERSE_PROMPT.)
        GenericValue::array_in_arena(
            self.into_iter().map(|t| t.into_value()).collect(),
            (1usize << <W as Word>::BITS_LOG2) / 8,
            (1usize << <F as Float>::BITS_LOG2) / 8,
            ctx.arena,
        )
        .map_err(|_| {
            VmError::OutOfArena(alloc::string::String::from(
                "arena exhausted building a native array result",
            ))
        })
    }

    fn flat_byte_size(word_bytes: usize, float_bytes: usize) -> Option<usize> {
        Some(N * <T as KeleusmaType<W, F>>::flat_byte_size(word_bytes, float_bytes)?)
    }

    fn from_flat_bytes(
        bytes: &[u8],
        word_bytes: usize,
        float_bytes: usize,
    ) -> Result<Self, VmError> {
        let esize = <T as KeleusmaType<W, F>>::flat_byte_size(word_bytes, float_bytes).ok_or_else(
            || {
                VmError::TypeError(alloc::string::String::from(
                    "flat array element type is not flat-eligible",
                ))
            },
        )?;
        // A zero-size element (`Unit`) stores no bytes, so the length is not
        // byte-derivable; trust the static `N`.
        let len = bytes.len().checked_div(esize).unwrap_or(N);
        if len != N {
            return Err(VmError::TypeError(format!(
                "expected array of length {}, got {}",
                N, len
            )));
        }
        let mut converted: Vec<T> = Vec::with_capacity(N);
        for i in 0..N {
            let lo = i * esize;
            converted.push(<T as KeleusmaType<W, F>>::from_flat_bytes(
                &bytes[lo..lo + esize],
                word_bytes,
                float_bytes,
            )?);
        }
        converted
            .try_into()
            .map_err(|_| VmError::TypeError(format!("failed to convert array of length {}", N)))
    }

    fn from_value_ctx(v: &GenericValue<W, F>, ctx: &RefContext<'_>) -> Result<Self, VmError> {
        use crate::bytecode::ArrayBody;
        match v {
            GenericValue::Array(ArrayBody::Boxed(items)) => {
                if items.len() != N {
                    return Err(VmError::TypeError(format!(
                        "expected array of length {}, got {}",
                        N,
                        items.len()
                    )));
                }
                let mut converted: Vec<T> = Vec::with_capacity(N);
                for item in items.iter() {
                    converted.push(T::from_value_ctx(item, ctx)?);
                }
                converted.try_into().map_err(|_| {
                    VmError::TypeError(format!("failed to convert array of length {}", N))
                })
            }
            GenericValue::Array(ArrayBody::Flat(fc)) => {
                // Resolve against the arena rather than assuming an `Inline`
                // body, so decode works on an arena-resident value (a yielded
                // or returned composite under the read-before-resume contract,
                // B28 P3 item 5 C3). A stale body (read after a RESET) is a
                // clean error, not a panic.
                let bytes = fc.resolve(ctx.arena).map_err(|_| stale_flat_decode())?;
                Self::from_flat_bytes_ctx(bytes, ctx.word_bytes, ctx.float_bytes, ctx)
            }
            other => Err(VmError::TypeError(format!(
                "expected array, got {}",
                other.type_name()
            ))),
        }
    }

    fn from_flat_bytes_ctx(
        bytes: &[u8],
        word_bytes: usize,
        float_bytes: usize,
        ctx: &RefContext<'_>,
    ) -> Result<Self, VmError> {
        let esize = <T as KeleusmaType<W, F>>::flat_byte_size(word_bytes, float_bytes).ok_or_else(
            || {
                VmError::TypeError(alloc::string::String::from(
                    "flat array element type is not flat-eligible",
                ))
            },
        )?;
        let len = bytes.len().checked_div(esize).unwrap_or(N);
        if len != N {
            return Err(VmError::TypeError(format!(
                "expected array of length {}, got {}",
                N, len
            )));
        }
        let mut converted: Vec<T> = Vec::with_capacity(N);
        for i in 0..N {
            let lo = i * esize;
            converted.push(<T as KeleusmaType<W, F>>::from_flat_bytes_ctx(
                &bytes[lo..lo + esize],
                word_bytes,
                float_bytes,
                ctx,
            )?);
        }
        converted
            .try_into()
            .map_err(|_| VmError::TypeError(format!("failed to convert array of length {}", N)))
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
                    // A flat tuple body is pure bytes; the Rust element types
                    // supply their flat sizes so each field (a scalar or a
                    // nested flat composite) is read at its packed offset
                    // (B28 P2). Runtime widths match the module widths on the
                    // bundled runtime.
                    GenericValue::Tuple(crate::bytecode::TupleBody::Flat(fc)) => {
                        let word_bytes = (1usize << <W as Word>::BITS_LOG2) / 8;
                        let float_bytes = (1usize << <FloatT as Float>::BITS_LOG2) / 8;
                        Self::from_flat_bytes(fc.as_bytes(), word_bytes, float_bytes)
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

            #[allow(non_snake_case)]
            fn into_value_ctx(self, __ctx: &RefContext<'_>)
                -> Result<GenericValue<W, FloatT>, VmError>
            {
                let ($($name,)*) = self;
                // Build the flat tuple body directly in the arena, eliminating
                // the top-level global-heap `Inline` (B28 P3 item 2, Increment
                // 3). Element values and packing widths are the host runtime's
                // own, so the body is byte-identical to `into_value` and
                // decodes the same through `from_value` (see the array impl on
                // the narrow-word module-width item).
                GenericValue::tuple_in_arena(
                    ::alloc::vec![$($name.into_value(),)*],
                    (1usize << <W as Word>::BITS_LOG2) / 8,
                    (1usize << <FloatT as Float>::BITS_LOG2) / 8,
                    __ctx.arena,
                )
                .map_err(|_| {
                    VmError::OutOfArena(::alloc::string::String::from(
                        "arena exhausted building a native tuple result",
                    ))
                })
            }

            #[allow(unused_assignments, unused_mut, unused_variables)]
            fn flat_byte_size(word_bytes: usize, float_bytes: usize) -> Option<usize> {
                let mut total = 0usize;
                $(
                    total += <$name as KeleusmaType<W, FloatT>>::flat_byte_size(word_bytes, float_bytes)?;
                )*
                Some(total)
            }

            #[allow(unused_assignments, unused_mut, unused_variables, non_snake_case)]
            fn from_flat_bytes(bytes: &[u8], word_bytes: usize, float_bytes: usize)
                -> Result<Self, VmError>
            {
                let mut offset = 0usize;
                // Tuple elements evaluate left-to-right, so the running
                // offset advances in declaration order (B28 P2).
                Ok(($(
                    {
                        let size = <$name as KeleusmaType<W, FloatT>>::flat_byte_size(word_bytes, float_bytes)
                            .ok_or_else(|| VmError::TypeError(::alloc::string::String::from(
                                "flat tuple field is not flat-eligible",
                            )))?;
                        let val = <$name as KeleusmaType<W, FloatT>>::from_flat_bytes(
                            &bytes[offset..offset + size], word_bytes, float_bytes,
                        )?;
                        offset += size;
                        val
                    },
                )*))
            }

            #[allow(clippy::unused_unit, unused_assignments, non_snake_case)]
            fn from_value_ctx(v: &GenericValue<W, FloatT>, __ctx: &RefContext<'_>) -> Result<Self, VmError> {
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
                        Ok(($($name::from_value_ctx(&items[$idx], __ctx)?,)*))
                    }
                    GenericValue::Tuple(crate::bytecode::TupleBody::Flat(fc)) => {
                        let bytes = fc.resolve(__ctx.arena).map_err(|_| crate::marshall::stale_flat_decode())?;
                        Self::from_flat_bytes_ctx(bytes, __ctx.word_bytes, __ctx.float_bytes, __ctx)
                    }
                    other => Err(VmError::TypeError(format!(
                        "expected tuple, got {}",
                        other.type_name()
                    ))),
                }
            }

            #[allow(unused_assignments, unused_mut, unused_variables, non_snake_case)]
            fn from_flat_bytes_ctx(bytes: &[u8], word_bytes: usize, float_bytes: usize, __ctx: &RefContext<'_>)
                -> Result<Self, VmError>
            {
                let mut offset = 0usize;
                Ok(($(
                    {
                        let size = <$name as KeleusmaType<W, FloatT>>::flat_byte_size(word_bytes, float_bytes)
                            .ok_or_else(|| VmError::TypeError(::alloc::string::String::from(
                                "flat tuple field is not flat-eligible",
                            )))?;
                        let val = <$name as KeleusmaType<W, FloatT>>::from_flat_bytes_ctx(
                            &bytes[offset..offset + size], word_bytes, float_bytes, __ctx,
                        )?;
                        offset += size;
                        val
                    },
                )*))
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
                    move |__ctx: &crate::vm::NativeCtx<'_>, args: &[GenericValue<W, FloatT>]|
                        -> Result<GenericValue<W, FloatT>, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        // Resolve reference (Text, opaque) fields of a
                        // composite argument through the VM context (B28 P3).
                        let __rc = __ctx.ref_context();
                        let _ = &__rc;
                        $(
                            let $name = <$name as KeleusmaType<W, FloatT>>::from_value_ctx(&args[$idx], &__rc)?;
                        )*
                        // Build the result's composite body directly in the
                        // arena through the producing `_ctx` family, so a native
                        // composite return carries no global-heap body (B28 P3
                        // item 2, Increment 3). The VM's later `into_arena_body`
                        // is then a no-op on this already-arena result.
                        <R as KeleusmaType<W, FloatT>>::into_value_ctx(self($($name,)*), &__rc)
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
                    move |__ctx: &crate::vm::NativeCtx<'_>, args: &[GenericValue<W, FloatT>]|
                        -> Result<GenericValue<W, FloatT>, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        let __rc = __ctx.ref_context();
                        let _ = &__rc;
                        $(
                            let $name = <$name as KeleusmaType<W, FloatT>>::from_value_ctx(&args[$idx], &__rc)?;
                        )*
                        // Arena-direct result body on the Ok path (B28 P3 item
                        // 2, Increment 3); the Err path surfaces the host error.
                        self($($name,)*)
                            .and_then(|__r| {
                                <R as KeleusmaType<W, FloatT>>::into_value_ctx(__r, &__rc)
                            })
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
        // Every element (including the Float) is flat-eligible (B28 P3 item 5),
        // so the tuple marshals to the flat byte body, not a boxed `Vec`.
        assert!(matches!(
            &v,
            Value::Tuple(crate::bytecode::TupleBody::Flat(_))
        ));
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
    fn scalar_array_into_value_uses_flat_body() {
        // A transitively-scalar array marshals to the flat byte body
        // (B28 P2), so the host and the runtime agree on representation.
        use crate::bytecode::ArrayBody;
        let v = <[i64; 3] as KeleusmaType<i64, f64>>::into_value([1, 2, 3]);
        assert!(matches!(v, Value::Array(ArrayBody::Flat(_))));
    }

    #[test]
    fn byte_array_roundtrips_through_flat_body() {
        // A one-byte element kind exercises the flat read/write offset
        // arithmetic at a non-word stride (B28 P2).
        use crate::bytecode::ArrayBody;
        let a: [u8; 4] = [1, 2, 250, 255];
        let v = <[u8; 4] as KeleusmaType<i64, f64>>::into_value(a);
        assert!(matches!(v, Value::Array(ArrayBody::Flat(_))));
        let r: [u8; 4] = <[u8; 4] as KeleusmaType<i64, f64>>::from_value(&v).unwrap();
        assert_eq!(r, [1, 2, 250, 255]);
    }

    #[test]
    fn struct_value_chooses_flat_or_boxed_body() {
        // An all-scalar struct uses the flat byte body; a reference field
        // forces the boxed body (B28 P2).
        use crate::bytecode::StructBody;
        let flat = Value::struct_value(
            ::alloc::string::String::from("P"),
            ::alloc::vec![
                (::alloc::string::String::from("a"), Value::Int(1)),
                (::alloc::string::String::from("b"), Value::Int(2)),
            ],
        );
        assert!(matches!(flat, Value::Struct(StructBody::Flat(_))));
        let boxed = Value::struct_value(
            ::alloc::string::String::from("Q"),
            ::alloc::vec![(
                ::alloc::string::String::from("s"),
                Value::StaticStr(::alloc::string::String::from("x")),
            )],
        );
        assert!(matches!(boxed, Value::Struct(StructBody::Boxed { .. })));
    }

    #[test]
    fn reference_element_array_uses_boxed_body() {
        // A reference-typed element (static string) is not flat-eligible,
        // so the array stays boxed (B28 P2 interim, matching tuples).
        use crate::bytecode::ArrayBody;
        let v = Value::array(::alloc::vec![
            Value::StaticStr(::alloc::string::String::from("a")),
            Value::StaticStr(::alloc::string::String::from("b")),
        ]);
        assert!(matches!(v, Value::Array(ArrayBody::Boxed(_))));
    }

    #[test]
    fn array_length_mismatch() {
        let v = Value::array(::alloc::vec![Value::Int(1), Value::Int(2)]);
        let err = <[i64; 3] as KeleusmaType<i64, f64>>::from_value(&v).unwrap_err();
        match err {
            VmError::TypeError(msg) => assert!(msg.contains("length")),
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    fn ctx(arena: &keleusma_arena::Arena) -> crate::vm::NativeCtx<'_> {
        crate::vm::NativeCtx {
            arena,
            opaques: &[],
            word_bytes: 8,
            float_bytes: 8,
        }
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
