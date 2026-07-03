//! Canonical zero value and lowest-valid resolution.
//!
//! B35 (Partial Operation Handling) phase P2. This module defines a
//! single canonical zero value for every type and the precedence by
//! which a refined newtype's lowest valid value is resolved. Native
//! code generation (B35 P8) is the intended consumer: where an
//! unhandled partial operation has no in-band result on a target, it
//! substitutes the canonical zero value of the relevant type. The
//! virtual machine traps instead, so this module has no runtime
//! consumer yet and is parallel infrastructure, in the same sense as
//! the B28 P0 and P1 layout scaffolding.
//!
//! The functions are pure and operate on a
//! [`crate::zero_value::TypeRegistry`] of the program's declarations,
//! so they are testable in isolation and do not depend on the full
//! compiler context.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ast::{EnumDef, Expr, NewtypeDef, PrimType, StructDef, TypeExpr};
use crate::bytecode::ConstValue;
use crate::interval::IntervalSet;

/// Recursion guard for `zero_value`. The value type system admits no
/// unbounded recursion because every composite has a statically known
/// finite size, so a well-formed program never approaches this depth.
/// The guard is a defensive bound against a malformed or cyclic
/// registry rather than an expected limit.
const MAX_DEPTH: usize = 64;

/// Failure modes of [`zero_value`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroValueError {
    /// A `Named` type was not present in the registry.
    UnknownType(String),
    /// An enum declaration carried no variants, so it has no
    /// canonical zero value.
    EmptyEnum(String),
    /// The recursion guard tripped, indicating a cyclic or malformed
    /// registry.
    RecursionLimit,
}

/// The program declarations that [`zero_value`] and [`lowest_valid`]
/// consult. The caller borrows its existing declaration maps; native
/// code generation already holds equivalents.
pub struct TypeRegistry<'a> {
    /// Struct declarations by name.
    pub structs: &'a BTreeMap<String, StructDef>,
    /// Enum declarations by name.
    pub enums: &'a BTreeMap<String, EnumDef>,
    /// Newtype declarations by name.
    pub newtypes: &'a BTreeMap<String, NewtypeDef>,
    /// Refinement predicate bodies by predicate function name, each a
    /// pair of the parameter name and the predicate body expression.
    /// Mirrors the compiler's `refinement_bodies` map.
    pub refinement_bodies: &'a BTreeMap<String, (String, Expr)>,
}

/// The canonical zero value of `ty`.
///
/// Scalars take their natural zero. `Text` is the empty string. A
/// tuple, struct, or array is its elements' zero values. An `Option`
/// is `None`. An enum is its zero-discriminant variant, or its
/// lowest-discriminant variant when no variant has discriminant zero,
/// constructed with zero payloads. A refined newtype is its lowest
/// valid value when one is determinable, and otherwise the hard zero
/// of its underlying type even if that violates the refinement, which
/// is the native default for the case with no determinable lowest
/// valid value.
pub fn zero_value(ty: &TypeExpr, reg: &TypeRegistry) -> Result<ConstValue, ZeroValueError> {
    zero_value_at(ty, reg, 0)
}

fn zero_value_at(
    ty: &TypeExpr,
    reg: &TypeRegistry,
    depth: usize,
) -> Result<ConstValue, ZeroValueError> {
    if depth > MAX_DEPTH {
        return Err(ZeroValueError::RecursionLimit);
    }
    match ty {
        TypeExpr::Unit(_) => Ok(ConstValue::Unit),
        TypeExpr::Prim(p, _) => Ok(zero_prim(p)),
        TypeExpr::Multiword(n, _) => Ok(ConstValue::Array(
            alloc::vec![zero_prim(&crate::ast::PrimType::Word); *n as usize],
        )),
        TypeExpr::Option(_, _) => Ok(ConstValue::None),
        TypeExpr::Tuple(elems, _) => {
            let mut out = Vec::with_capacity(elems.len());
            for e in elems {
                out.push(zero_value_at(e, reg, depth + 1)?);
            }
            Ok(ConstValue::Tuple(out))
        }
        TypeExpr::Array(elem, n, _) => {
            let count = (*n).max(0) as usize;
            let z = zero_value_at(elem, reg, depth + 1)?;
            Ok(ConstValue::Array(alloc::vec![z; count]))
        }
        TypeExpr::Named(name, _generics, _) => zero_named(name, reg, depth),
        // Information-flow labels do not affect the runtime value;
        // descend to the inner type, as the layout pass does.
        TypeExpr::Labelled(inner, _, _) | TypeExpr::NegativeLabelled(inner, _, _) => {
            zero_value_at(inner, reg, depth + 1)
        }
    }
}

/// The canonical zero value of a primitive type.
fn zero_prim(p: &PrimType) -> ConstValue {
    match p {
        PrimType::Word => ConstValue::Int(0),
        PrimType::Byte => ConstValue::Byte(0),
        PrimType::Fixed(_) => ConstValue::Fixed(0),
        PrimType::Bool => ConstValue::Bool(false),
        PrimType::Text => ConstValue::StaticStr(String::new()),
        #[cfg(feature = "floats")]
        PrimType::Float => ConstValue::Float(0.0),
        // Without the `floats` feature a `Float` type cannot appear in
        // a well-formed program, so a zero `Word` is an unreachable
        // defensive placeholder.
        #[cfg(not(feature = "floats"))]
        PrimType::Float => ConstValue::Int(0),
    }
}

fn zero_named(name: &str, reg: &TypeRegistry, depth: usize) -> Result<ConstValue, ZeroValueError> {
    if let Some(nt) = reg.newtypes.get(name) {
        if let Some(v) = lowest_valid(nt, reg) {
            return wrap_underlying(&nt.underlying, v, reg, depth);
        }
        // No determinable lowest valid value: the native default is
        // the hard zero of the underlying type, accepted even if it
        // violates the refinement predicate.
        return zero_value_at(&nt.underlying, reg, depth + 1);
    }
    if let Some(sd) = reg.structs.get(name) {
        let mut fields = Vec::with_capacity(sd.fields.len());
        for f in &sd.fields {
            fields.push((f.name.clone(), zero_value_at(&f.type_expr, reg, depth + 1)?));
        }
        return Ok(ConstValue::Struct {
            type_name: String::from(name),
            fields,
        });
    }
    if let Some(ed) = reg.enums.get(name) {
        // Prefer the zero-discriminant variant; otherwise the variant
        // with the lowest discriminant.
        let variant = ed
            .variants
            .iter()
            .find(|v| v.discriminant_value == 0)
            .or_else(|| ed.variants.iter().min_by_key(|v| v.discriminant_value))
            .ok_or_else(|| ZeroValueError::EmptyEnum(String::from(name)))?;
        let mut fields = Vec::with_capacity(variant.fields.len());
        for t in &variant.fields {
            fields.push(zero_value_at(t, reg, depth + 1)?);
        }
        return Ok(ConstValue::Enum {
            type_name: String::from(name),
            variant: variant.name.clone(),
            discriminant: Some(variant.discriminant_value),
            fields,
        });
    }
    Err(ZeroValueError::UnknownType(String::from(name)))
}

/// Wrap a resolved lowest-valid integer in the `ConstValue` for the
/// newtype's underlying type. Refinements and saturation values are
/// integer-domain, so this handles `Word`, `Byte`, and `Fixed`
/// underlying types; any other underlying type ignores the integer
/// and falls back to the underlying zero.
fn wrap_underlying(
    underlying: &TypeExpr,
    v: i64,
    reg: &TypeRegistry,
    depth: usize,
) -> Result<ConstValue, ZeroValueError> {
    match underlying {
        TypeExpr::Prim(PrimType::Word, _) => Ok(ConstValue::Int(v)),
        TypeExpr::Prim(PrimType::Byte, _) => Ok(ConstValue::Byte(v.clamp(0, 0xFF) as u8)),
        TypeExpr::Prim(PrimType::Fixed(_), _) => Ok(ConstValue::Fixed(v)),
        _ => zero_value_at(underlying, reg, depth + 1),
    }
}

/// The lowest valid value of a refined newtype, in the integer
/// domain, resolved by the B35 precedence.
///
/// 1. The declared `with saturate_min`, which the grammar verifies
///    against the refinement predicate.
/// 2. The minimum of the predicate's true set, when the interval and
///    lattice analysis can bound it below.
/// 3. `None`, leaving the caller to trap on the virtual machine or
///    substitute a hard zero on a native target.
pub fn lowest_valid(nt: &NewtypeDef, reg: &TypeRegistry) -> Option<i64> {
    if let Some(m) = nt.saturate_min {
        return Some(m);
    }
    let pred_name = nt.refinement.as_ref()?;
    let (param, body) = reg.refinement_bodies.get(pred_name)?;
    let set = crate::compiler::predicate_true_set(body, param)?;
    interval_set_min(&set)
}

/// The minimum value of an interval set, or `None` when the set is
/// empty or unbounded below. The set's parts are normalized in
/// ascending order, so the first part carries the global minimum.
fn interval_set_min(set: &IntervalSet) -> Option<i64> {
    set.parts().first().and_then(|iv| iv.lo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinOp, FieldDecl, Literal, VariantDecl};
    use crate::token::Span;

    fn sp() -> Span {
        Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        }
    }

    fn prim(p: PrimType) -> TypeExpr {
        TypeExpr::Prim(p, sp())
    }

    #[allow(clippy::type_complexity)]
    fn empty_reg() -> (
        BTreeMap<String, StructDef>,
        BTreeMap<String, EnumDef>,
        BTreeMap<String, NewtypeDef>,
        BTreeMap<String, (String, Expr)>,
    ) {
        (
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    fn reg<'a>(
        s: &'a BTreeMap<String, StructDef>,
        e: &'a BTreeMap<String, EnumDef>,
        n: &'a BTreeMap<String, NewtypeDef>,
        r: &'a BTreeMap<String, (String, Expr)>,
    ) -> TypeRegistry<'a> {
        TypeRegistry {
            structs: s,
            enums: e,
            newtypes: n,
            refinement_bodies: r,
        }
    }

    #[test]
    fn scalar_zeros() {
        let (s, e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        assert_eq!(
            zero_value(&prim(PrimType::Word), &reg),
            Ok(ConstValue::Int(0))
        );
        assert_eq!(
            zero_value(&prim(PrimType::Byte), &reg),
            Ok(ConstValue::Byte(0))
        );
        assert_eq!(
            zero_value(&prim(PrimType::Fixed(Some(16))), &reg),
            Ok(ConstValue::Fixed(0))
        );
        assert_eq!(
            zero_value(&prim(PrimType::Bool), &reg),
            Ok(ConstValue::Bool(false))
        );
        assert_eq!(
            zero_value(&prim(PrimType::Text), &reg),
            Ok(ConstValue::StaticStr(String::new()))
        );
        assert_eq!(
            zero_value(&TypeExpr::Unit(sp()), &reg),
            Ok(ConstValue::Unit)
        );
    }

    #[test]
    fn option_zero_is_none() {
        let (s, e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let ty = TypeExpr::Option(alloc::boxed::Box::new(prim(PrimType::Word)), sp());
        assert_eq!(zero_value(&ty, &reg), Ok(ConstValue::None));
    }

    #[test]
    fn tuple_and_array_zeros() {
        let (s, e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let tup = TypeExpr::Tuple(
            alloc::vec![prim(PrimType::Word), prim(PrimType::Bool)],
            sp(),
        );
        assert_eq!(
            zero_value(&tup, &reg),
            Ok(ConstValue::Tuple(alloc::vec![
                ConstValue::Int(0),
                ConstValue::Bool(false)
            ]))
        );
        let arr = TypeExpr::Array(alloc::boxed::Box::new(prim(PrimType::Byte)), 3, sp());
        assert_eq!(
            zero_value(&arr, &reg),
            Ok(ConstValue::Array(alloc::vec![
                ConstValue::Byte(0),
                ConstValue::Byte(0),
                ConstValue::Byte(0)
            ]))
        );
    }

    #[test]
    fn struct_zero_recurses_fields() {
        let mut s = BTreeMap::new();
        s.insert(
            String::from("Point"),
            StructDef {
                name: String::from("Point"),
                type_params: Vec::new(),
                fields: alloc::vec![
                    FieldDecl {
                        name: String::from("x"),
                        type_expr: prim(PrimType::Word),
                        span: sp(),
                    },
                    FieldDecl {
                        name: String::from("ok"),
                        type_expr: prim(PrimType::Bool),
                        span: sp(),
                    },
                ],
                span: sp(),
            },
        );
        let (_s, e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let z = zero_value(
            &TypeExpr::Named(String::from("Point"), Vec::new(), sp()),
            &reg,
        )
        .unwrap();
        assert_eq!(
            z,
            ConstValue::Struct {
                type_name: String::from("Point"),
                fields: alloc::vec![
                    (String::from("x"), ConstValue::Int(0)),
                    (String::from("ok"), ConstValue::Bool(false)),
                ],
            }
        );
    }

    fn unit_variant(name: &str, disc: i64) -> VariantDecl {
        VariantDecl {
            name: String::from(name),
            fields: Vec::new(),
            explicit_discriminant: Some(disc),
            discriminant_value: disc,
            span: sp(),
        }
    }

    #[test]
    fn enum_zero_prefers_zero_discriminant() {
        let mut e = BTreeMap::new();
        e.insert(
            String::from("Dir"),
            EnumDef {
                name: String::from("Dir"),
                type_params: Vec::new(),
                variants: alloc::vec![unit_variant("North", 0), unit_variant("South", 1)],
                span: sp(),
            },
        );
        let (s, _e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let z = zero_value(
            &TypeExpr::Named(String::from("Dir"), Vec::new(), sp()),
            &reg,
        )
        .unwrap();
        assert_eq!(
            z,
            ConstValue::Enum {
                type_name: String::from("Dir"),
                variant: String::from("North"),
                discriminant: Some(0),
                fields: Vec::new(),
            }
        );
    }

    #[test]
    fn enum_zero_falls_back_to_lowest_discriminant() {
        let mut e = BTreeMap::new();
        e.insert(
            String::from("Code"),
            EnumDef {
                name: String::from("Code"),
                type_params: Vec::new(),
                variants: alloc::vec![unit_variant("B", 8), unit_variant("A", 5)],
                span: sp(),
            },
        );
        let (s, _e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let z = zero_value(
            &TypeExpr::Named(String::from("Code"), Vec::new(), sp()),
            &reg,
        )
        .unwrap();
        match z {
            ConstValue::Enum { variant, .. } => assert_eq!(variant, "A"),
            other => panic!("expected enum, got {:?}", other),
        }
    }

    fn newtype(name: &str, refinement: Option<&str>, sat_min: Option<i64>) -> NewtypeDef {
        NewtypeDef {
            name: String::from(name),
            underlying: prim(PrimType::Word),
            refinement: refinement.map(String::from),
            saturate_max: None,
            saturate_min: sat_min,
            span: sp(),
        }
    }

    #[test]
    fn newtype_lowest_valid_prefers_declared_saturate_min() {
        let mut n = BTreeMap::new();
        n.insert(String::from("Limited"), newtype("Limited", None, Some(5)));
        let (s, e, _n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let nt = &n[&String::from("Limited")];
        assert_eq!(lowest_valid(nt, &reg), Some(5));
        assert_eq!(
            zero_value(
                &TypeExpr::Named(String::from("Limited"), Vec::new(), sp()),
                &reg
            ),
            Ok(ConstValue::Int(5))
        );
    }

    #[test]
    fn newtype_lowest_valid_from_predicate_interval() {
        // Predicate `x >= 5`, expressed as the body expression the
        // compiler's predicate analysis understands.
        let body = Expr::BinOp {
            op: BinOp::GtEq,
            left: alloc::boxed::Box::new(Expr::Ident {
                name: String::from("x"),
                span: sp(),
            }),
            right: alloc::boxed::Box::new(Expr::Literal {
                value: Literal::Int(5),
                span: sp(),
            }),
            span: sp(),
        };
        let mut r = BTreeMap::new();
        r.insert(String::from("at_least_5"), (String::from("x"), body));
        let mut n = BTreeMap::new();
        n.insert(
            String::from("Big"),
            newtype("Big", Some("at_least_5"), None),
        );
        let (s, e, _n, _r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let nt = &n[&String::from("Big")];
        assert_eq!(lowest_valid(nt, &reg), Some(5));
    }

    #[test]
    fn newtype_without_lower_bound_falls_back_to_hard_zero() {
        // No saturate_min and no refinement: the lowest valid value is
        // undeterminable, so the canonical zero is the hard zero of
        // the underlying Word.
        let mut n = BTreeMap::new();
        n.insert(String::from("Raw"), newtype("Raw", None, None));
        let (s, e, _n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        let nt = &n[&String::from("Raw")];
        assert_eq!(lowest_valid(nt, &reg), None);
        assert_eq!(
            zero_value(
                &TypeExpr::Named(String::from("Raw"), Vec::new(), sp()),
                &reg
            ),
            Ok(ConstValue::Int(0))
        );
    }

    #[test]
    fn unknown_named_type_errors() {
        let (s, e, n, r) = empty_reg();
        let reg = reg(&s, &e, &n, &r);
        assert_eq!(
            zero_value(
                &TypeExpr::Named(String::from("Nope"), Vec::new(), sp()),
                &reg
            ),
            Err(ZeroValueError::UnknownType(String::from("Nope")))
        );
    }
}
