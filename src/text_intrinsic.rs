//! The bundled text operations over `Text<N>`, recognised by name.
//!
//! The operations are compiler intrinsics, ruled by the operator on 2026-10-31
//! as option (b) over generic natives and per-capacity natives. A call such as
//! `text::concat(a, b)` is typed by the type checker and lowered by the
//! compiler to existing opcodes. No native is registered, no opcode is added,
//! and no host sees the call.
//!
//! The `text` path is reserved. In a build with the `text` cargo feature a call
//! under it is an intrinsic, and in a build without the feature the same call
//! is refused at compile time rather than resolved as a host native. A `use`
//! declaration under the path is refused in both builds, because importing a
//! host native under the intrinsic namespace would make one spelling mean two
//! things depending on how the runtime was built.
//!
//! See `docs/decisions/TEXT_CAPACITY_TYPE.md`, the section on the bundled
//! operations, for the operation list, the result capacities, the lowering and
//! the cost of each.

extern crate alloc;

use alloc::format;
use alloc::string::String;

/// The path segment that names the intrinsic namespace.
pub(crate) const NAMESPACE: &str = "text";

/// Whether the intrinsics are compiled into this build.
pub(crate) const ENABLED: bool = cfg!(feature = "text");

/// One bundled text operation.
///
/// Without the `text` feature nothing consumes an operation beyond its name
/// lookup, so the accessors are unused there by design.
#[cfg_attr(not(feature = "text"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextOp {
    /// `text::len(t: Text<N>) -> Word`, the live byte length.
    Len,
    /// `text::capacity(t: Text<N>) -> Word`, the static capacity `N`.
    Capacity,
    /// `text::concat(a: Text<A>, b: Text<B>) -> Text<A + B>`.
    Concat,
    /// `text::eq(a: Text<A>, b: Text<B>) -> Bool`, equality of live content.
    Eq,
    /// `text::slice(t: Text<N>, start: Word, end: Word) -> Text<N>`, the
    /// half-open byte range clamped to the live length.
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

    /// The unqualified name of the operation.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TextOp::Len => "len",
            TextOp::Capacity => "capacity",
            TextOp::Concat => "concat",
            TextOp::Eq => "eq",
            TextOp::Slice => "slice",
        }
    }

    /// The number of arguments the operation takes.
    pub(crate) fn arity(self) -> usize {
        match self {
            TextOp::Len | TextOp::Capacity => 1,
            TextOp::Concat | TextOp::Eq => 2,
            TextOp::Slice => 3,
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

/// The unqualified remainder of a call name under the intrinsic namespace, or
/// `None` when the name is not under it.
///
/// Only a two-segment name qualifies. `text::concat` is under the namespace,
/// while `text` alone, `texts::concat` and `a::text::concat` are not.
pub(crate) fn namespace_member(call_name: &str) -> Option<&str> {
    let rest = call_name.strip_prefix(NAMESPACE)?.strip_prefix("::")?;
    if rest.is_empty() || rest.contains("::") {
        return None;
    }
    Some(rest)
}

/// Whether a `use` path names the intrinsic namespace as its first segment.
pub(crate) fn use_path_is_reserved(path: &[String]) -> bool {
    path.first().map(String::as_str) == Some(NAMESPACE)
}

/// Resolve a call name under the namespace to its operation.
///
/// `Ok(None)` when the name is not under the namespace, so the caller resolves
/// it as before. `Err` carries the refusal message when the name is under the
/// namespace but this build does not have the feature, or when the member is
/// not an operation.
pub(crate) fn resolve(call_name: &str) -> Result<Option<TextOp>, String> {
    let Some(member) = namespace_member(call_name) else {
        return Ok(None);
    };
    if !ENABLED {
        return Err(format!(
            "`{call_name}` is a bundled text operation, and this build was compiled without the \
             `text` cargo feature that provides it"
        ));
    }
    match TextOp::from_name(member) {
        Some(op) => Ok(Some(op)),
        None => Err(format!(
            "`{call_name}` is not a bundled text operation. The operations are text::len, \
             text::capacity, text::concat, text::eq and text::slice"
        )),
    }
}

/// The refusal message for a `use` declaration under the namespace.
pub(crate) fn reserved_use_message(path: &[String]) -> String {
    format!(
        "`use {}::...` is refused: the `{NAMESPACE}` path names the bundled text operations, \
         which are called directly and are never imported as host natives",
        path.join("::")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_two_segment_name_is_under_the_namespace() {
        assert_eq!(namespace_member("text::concat"), Some("concat"));
        assert_eq!(namespace_member("text::anything"), Some("anything"));
        for outside in [
            "text",
            "text::",
            "texts::concat",
            "a::text::concat",
            "text::a::b",
        ] {
            assert_eq!(namespace_member(outside), None, "{outside}");
        }
    }

    #[test]
    fn every_operation_round_trips_through_its_name() {
        for op in TextOp::ALL {
            assert_eq!(TextOp::from_name(op.name()), Some(op));
        }
        assert_eq!(TextOp::from_name("length"), None);
    }

    #[test]
    fn a_name_outside_the_namespace_resolves_to_nothing() {
        assert_eq!(resolve("math::sqrt"), Ok(None));
        assert_eq!(resolve("concat"), Ok(None));
    }

    #[cfg(feature = "text")]
    #[test]
    fn a_name_inside_the_namespace_resolves_or_is_refused_by_name() {
        assert_eq!(resolve("text::concat"), Ok(Some(TextOp::Concat)));
        let err = resolve("text::length").unwrap_err();
        assert!(err.contains("not a bundled text operation"), "{err}");
    }

    #[cfg(not(feature = "text"))]
    #[test]
    fn without_the_feature_every_name_inside_the_namespace_is_refused() {
        for name in ["text::concat", "text::len", "text::length"] {
            let err = resolve(name).unwrap_err();
            assert!(err.contains("`text` cargo feature"), "{err}");
        }
    }
}
