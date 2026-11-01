//! The bundled text operations over `Text<N>`, recognised as methods on the
//! value.
//!
//! The operations are compiler intrinsics, ruled by the operator on 2026-10-31
//! as option (b) over generic natives and per-capacity natives. The operator
//! ruled the same day that they are called as methods, `t.len()`,
//! `t.capacity()`, `a.concat(b)`, `a.eq(b)` and `t.slice(start, end)`, and that
//! the method form replaces the earlier free functions so that each operation
//! has one spelling. A call is typed by the type checker and lowered by the
//! compiler to existing opcodes. No native is registered, no opcode is added,
//! and no host sees the call.
//!
//! # Resolution rule
//!
//! A method call is an operation exactly when its receiver has the type
//! `Text<N>` and the method is one of the five names. The receiver type
//! decides, not a trait impl, and the check comes before trait resolution.
//! A method of the same name on any other receiver resolves through its trait
//! impl as before, so no user type loses a method. A trait impl for a
//! `Text<N>` cannot supply one of the five names to a method call, which no
//! program could do before either, because trait resolution never found such
//! an impl for a `Text<N>` receiver. In a build without the `text` cargo
//! feature the same call is refused at compile time naming the feature.
//!
//! See `docs/decisions/TEXT_CAPACITY_TYPE.md`, the section on the bundled
//! operations, for the operation list, the result capacities, the defined
//! result for a malformed operand, the lowering and the cost of each.

extern crate alloc;

use alloc::format;
use alloc::string::String;

/// Whether the intrinsics are compiled into this build.
pub(crate) const ENABLED: bool = cfg!(feature = "text");

/// One bundled text operation.
///
/// Without the `text` feature nothing consumes an operation beyond its name
/// lookup, so the accessors are unused there by design.
#[cfg_attr(not(feature = "text"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextOp {
    /// `t.len() -> Word`, the effective byte length.
    Len,
    /// `t.capacity() -> Word`, the static capacity `N`.
    Capacity,
    /// `a.concat(b: Text<B>) -> Text<A + B>`.
    Concat,
    /// `a.eq(b: Text<B>) -> Bool`, equality of effective content.
    Eq,
    /// `t.slice(start: Word, end: Word) -> Text<N>`, the half-open byte range
    /// clamped to the effective length.
    Slice,
}

#[cfg_attr(not(feature = "text"), allow(dead_code))]
impl TextOp {
    /// Every operation, in documentation order.
    #[cfg(test)]
    pub(crate) const ALL: [TextOp; 5] = [
        TextOp::Len,
        TextOp::Capacity,
        TextOp::Concat,
        TextOp::Eq,
        TextOp::Slice,
    ];

    /// The method name of the operation.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TextOp::Len => "len",
            TextOp::Capacity => "capacity",
            TextOp::Concat => "concat",
            TextOp::Eq => "eq",
            TextOp::Slice => "slice",
        }
    }

    /// The number of arguments the method takes, excluding the receiver.
    pub(crate) fn arity(self) -> usize {
        match self {
            TextOp::Len | TextOp::Capacity => 0,
            TextOp::Concat | TextOp::Eq => 1,
            TextOp::Slice => 2,
        }
    }

    fn from_name(name: &str) -> Option<TextOp> {
        match name {
            "len" => Some(TextOp::Len),
            "capacity" => Some(TextOp::Capacity),
            "concat" => Some(TextOp::Concat),
            "eq" => Some(TextOp::Eq),
            "slice" => Some(TextOp::Slice),
            _ => None,
        }
    }
}

/// Resolve a method called on a `Text<N>` receiver to its operation.
///
/// The caller has already established that the receiver type is `Text<N>`.
/// `Ok(None)` when the method is not an operation, so the caller resolves it
/// through trait impls as before. `Err` carries the refusal message when the
/// method is an operation and this build does not have the feature.
pub(crate) fn resolve_method(method: &str) -> Result<Option<TextOp>, String> {
    let Some(op) = TextOp::from_name(method) else {
        return Ok(None);
    };
    if !ENABLED {
        return Err(format!(
            "`.{method}()` on a `Text<N>` is a bundled text operation, and this build was \
             compiled without the `text` cargo feature that provides it"
        ));
    }
    Ok(Some(op))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operation_round_trips_through_its_name() {
        for op in TextOp::ALL {
            assert_eq!(TextOp::from_name(op.name()), Some(op));
        }
        assert_eq!(TextOp::from_name("length"), None);
    }

    #[test]
    fn a_name_that_is_not_an_operation_resolves_to_nothing() {
        for name in ["length", "push", "", "text::len", "Len"] {
            assert_eq!(resolve_method(name), Ok(None), "{name}");
        }
    }

    #[cfg(feature = "text")]
    #[test]
    fn every_operation_name_resolves_with_the_feature() {
        for op in TextOp::ALL {
            assert_eq!(resolve_method(op.name()), Ok(Some(op)));
        }
    }

    #[cfg(not(feature = "text"))]
    #[test]
    fn without_the_feature_every_operation_name_is_refused() {
        for op in TextOp::ALL {
            let err = resolve_method(op.name()).unwrap_err();
            assert!(err.contains("`text` cargo feature"), "{err}");
        }
    }
}
